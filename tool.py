#!/bin/env python3

import sys

if len(sys.argv) < 5:
    sys.stderr.write("Usage: {} <email> <password> <course-id> <assignment-id>".format(sys.argv[0]))
    sys.exit(1)

email = sys.argv[1]
password = sys.argv[2]
course_id = sys.argv[3]
assignment_id = sys.argv[4]

sys.stderr.write("Loading... ")

from lxml import html, etree
import requests
from getpass import getpass, getuser
from os import path

# Start a session
session = requests.session()

# Get an authenticity token
login = html.fromstring(session.get('https://gradescope.com/login').content)
token = login.xpath('//form//input[@name="authenticity_token"]')[0].get('value')

sys.stderr.write("done.\n")

# Login
data = {
    'utf8': '✓',
    'authenticity_token': token,
    'session[email]': email,
    'session[password]': password,
    'session[remember_me]': '0',
    'commit': 'Log In',
    'session[remember_me_sso]': '0',
}
headers = {
    'Host': 'www.gradescope.com',
    'Referer': 'https://www.gradescope.com',
}
session.post("https://gradescope.com/login", data=data, headers=headers)

#print("Getting TA list... ", end='')
#sys.stdout.flush()
#
## Get a list of all TAs
#graders = []
#roster = html.fromstring(session.get("https://www.gradescope.com/courses/85770/memberships").content)
#for row in roster.xpath('//tr[contains(concat(" ",normalize-space(@class)," "),"rosterRow")]'):
#    name = row.xpath(".//td")[0].text
#    is_ta = row.xpath(".//td")[3].xpath(".//option")[2].get('selected') == 'selected'
#    if is_ta:
#        graders.append(name)
#
#print('done.')

sys.stderr.write("Getting assignment details... ")
sys.stderr.flush()

# Get assignment details
questions = {}
details = html.fromstring(session.get("https://www.gradescope.com/courses/{}/assignments/{}/grade".format(course_id, assignment_id)).content)
i = 1
for row in details.xpath('//div[@class="gradingDashboard--question" or @class="gradingDashboard--questionGroupContainer"]'):
    if row.attrib["class"] == "gradingDashboard--question":
        q_url = row.xpath('.//a[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--listAllLink")]')[0].get('href')
        q_id = q_url.split("/")[4]
        q_pts = float(row.xpath('.//div[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--pointsColumn")]')[0].text)
        q_num = float(row.xpath('.//div[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--questionTitle")]//a')[0].text.split(':')[0])
        i += 1
        questions[q_id] = {
            'url': q_url,
            'pts': q_pts,
            'graders': {},
            'num': q_num
        }
    else:
        for row in row.xpath('.//div[@class="gradingDashboard--subquestion"]'):
            q_url = row.xpath('.//a[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--listAllLink")]')[0].get('href')
            q_id = q_url.split("/")[4]
            q_pts = float(row.xpath('.//div[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--pointsColumn")]')[0].text)
            q_num = float(row.xpath('.//div[contains(concat(" ",normalize-space(@class)," "),"gradingDashboard--subquestionTitle")]//a')[0].text.split(':')[0])
            i += 1
            questions[q_id] = {
                'url': q_url,
                'pts': q_pts,
                'graders': {},
                'num': q_num
            }

if path.exists('.ignored_questions'):
    ns = []
    with open('.ignored_questions', 'r') as f:
        for line in f.read().strip().split('\n'):
            ns.append(float(line))

    for key in list(questions.keys())[::-1]:
        if questions[key]['num'] in ns:
            questions.pop(key)

#    ns.sort()
#    for n in ns[::-1]:
#        questions.pop(list(questions.keys())[n - 1])

sys.stderr.write('done.\n')
sys.stderr.write("Getting graders for each assignment... ")
sys.stderr.flush()

# Get the graders for each assignment
graders = []
i = 0
t = len(questions)
for q_id in questions:
    sys.stderr.write("\rGetting graders for each assignment... {}/{}".format(i, t))
    sys.stderr.flush()
    details = html.fromstring(session.get("https://www.gradescope.com/courses/{}/questions/{}/submissions".format(course_id, q_id)).content)
    for row in details.xpath('//*[@id="question_submissions"]//tr'):
        grader = row.xpath('.//td')
        if len(grader) > 2:
            if not grader[4].text:
                grader = "UNGRADED"
            else:
                grader = grader[2].text
            if not grader:
                grader = "AUTO"
            if grader in questions[q_id]['graders']:
                questions[q_id]['graders'][grader] += 1
            else:
                questions[q_id]['graders'][grader] = 1
            if grader != "UNGRADED" and grader not in graders:
                graders.append(grader)
    i += 1

sys.stderr.write("\rGetting graders for each assignment... done.\n")
sys.stderr.flush()

# Add missing graders
if path.exists('.staff'):
    with open('.staff', 'r') as f:
        lines = f.read().strip().split('\n')
    for g in lines:
        if g not in graders:
            graders.append(g)

# List stats
graders.sort()
graders.append("AUTO")
graders.append("UNGRADED")
print("{:25}".format("Name"), end='')
for q_id in questions:
    print("{:5}".format(questions[q_id]['num']), end='')

print("     ?s    Pts")
print("-" * (25 + len(questions) * 5 + 14))

grader_stats = {}

for grader in graders:
    grader_stats[grader] = {
        'qs': {}
    }
    t = 0
    p = 0
    for q_id in questions:
        if grader in questions[q_id]['graders']:
            count = questions[q_id]['graders'][grader]
        else:
            count = 0
        grader_stats[grader]['qs'][q_id] = count
        t += count
        p += count * questions[q_id]['pts']
    grader_stats[grader]['tot'] = t
    grader_stats[grader]['pts'] = int(p)

grader_stats = sorted(grader_stats.items(), key=lambda x: -x[1]['pts'])
grader_stats = sorted(grader_stats, key=lambda x: 2 if x[0] == "UNGRADED" else 1 if x[0] == "AUTO" else 0)

for grader in grader_stats:
    if grader[0] == "UNGRADED":
        print("-" * (25 + len(questions) * 5 + 14))
    print("{:25}".format(grader[0]), end='')
    for q_id in questions:
        print("{:5}".format(grader[1]['qs'][q_id]), end='')
    print("{:7}{:7}".format(grader[1]['tot'], grader[1]['pts']))
